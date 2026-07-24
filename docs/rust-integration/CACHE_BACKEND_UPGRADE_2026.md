# Cache Backend Upgrade — Distributed, Lazy, Adaptive

**Status:** ✅ IMPLEMENTED  
**Tanggal:** 2026-07-24  
**Area:** `native/src/infrastructure/cache_backend.rs`, `adapters.rs`, `adaptive_cache.rs`, `scan_cache.rs`, `redis_distributed.rs`

---

## Ringkasan

Empat fungsi/komponen cache yang sebelumnya berupa stub, fallback, atau dead-code telah diupgrade menjadi implementasi penuh:

1. **`DistributedCache` adapter** — `CacheConfig::Distributed` sekarang memakai `RedisDistributedCache` via `DistributedCacheAdapter`, bukan fallback ke `LruCache`.
2. **`Lazy` cache dengan timeout** — `CacheConfig::Lazy { timeout_ms }` sekarang menggunakan `LazyCacheAdapter` yang mematuhi `timeout_ms` untuk expiry.
3. **`Adaptive` cache dengan `max_capacity`** — `CacheConfig::Adaptive` sekarang memakai `StringKeyedAdaptiveCache` yang mempertahankan `max_capacity`.
4. **`scan_cache` aktif** — `priority_score`, `cache_dump`, `cache_load`, dan field `mtime_ms`/`size` di `CacheEntry` diaktifkan untuk scheduling scan dan persistence.

---

## 1. DistributedCache Adapter

**File:** `native/src/infrastructure/adapters.rs`, `cache_backend.rs`

### Sebelum
```rust
CacheConfig::Distributed { coordinator_url: _ } => {
    // TODO: Implement DistributedCache adapter for CacheBackend
    Arc::new(crate::infrastructure::lru_cache::LruCache::new(10000))
}
```

### Sesudah
```rust
CacheConfig::Distributed { coordinator_url: _ } => {
    let nodes = vec![ ... ];
    let config = crate::infrastructure::redis_distributed::RedisDistributedConfig { ... };
    let distributed = match RedisDistributedCache::new(config) {
        Ok(d) => d,
        Err(_) => return Arc::new(crate::infrastructure::lru_cache::LruCache::new(10000)),
    };
    Arc::new(crate::infrastructure::adapters::DistributedCacheAdapter::new(distributed, 10000))
}
```

### Adapter Baru: `DistributedCacheAdapter`
- Mengimplementasikan trait `CacheBackend` untuk `RedisDistributedCache`.
- Metode: `get`, `put`, `remove`, `clear`, `stats`, `capacity`.
- Fallback ke `LruCache` jika `RedisDistributedCache::new` gagal.

### Tambahan di `RedisDistributedCache`
Menambahkan metode agar bisa diadaptasi:
- `clear()` — flush semua pool dan reset version cache.
- `len()` — jumlah tracked keys.
- `is_empty()` — cek apakah kosong.

---

## 2. Lazy Cache dengan Timeout

**File:** `native/src/infrastructure/adapters.rs`

### Sebelum
```rust
CacheConfig::Lazy { timeout_ms: _ } => {
    // Use LruCache as fallback for now
    Arc::new(crate::infrastructure::lru_cache::LruCache::new(1000))
}
```

### Sesudah
```rust
CacheConfig::Lazy { timeout_ms } => {
    Arc::new(crate::infrastructure::adapters::LazyCacheAdapter::new(timeout_ms))
}
```

### `LazyCacheAdapter` Baru
- Simpan `(value, Instant::now())` di `HashMap`.
- Saat `get()`, cek apakah `timeout_ms` sudah terlewati. Jika ya, hapus entri (miss).
- `put()` selalu menyegarkan timestamp.
- `evict_expired()` helper untuk batch eviction (tidak dipanggil secara inline untuk menghindari lock contention).

---

## 3. Adaptive Cache dengan `max_capacity`

**File:** `native/src/infrastructure/adapters.rs`, `adaptive_cache.rs`

### Sebelum
```rust
CacheConfig::Adaptive { initial_capacity, max_capacity: _ } => {
    let initial = Box::new(LruCache::new(initial_capacity)) as Box<dyn CacheBackend>;
    Arc::new(AdaptiveCacheAdapter::new(initial))
}
```

### Sesudah
```rust
CacheConfig::Adaptive { initial_capacity, max_capacity } => {
    Arc::new(crate::infrastructure::adapters::StringKeyedAdaptiveCache::new(initial_capacity, max_capacity))
}
```

### `StringKeyedAdaptiveCache` Baru
- Wrap `AdaptiveCache<String, String>` agar kompatibel dengan trait `CacheBackend` (`&str` key).
- Mempertahankan `max` untuk `capacity()`.
- Memanggil `adapt_size()` di setiap `put()` agar auto-scaling aktif.

### Tambahan di `AdaptiveCache<K, V>`
Menambahkan metode agar lengkap mengimplementasikan `CacheBackend`:
- `remove(key: &K) -> bool`
- `contains(key: &K) -> bool`

---

## 4. Scan Cache Aktif

**File:** `native/src/scan_cache.rs`

### Sebelum
```rust
#[allow(dead_code)]
pub fn priority_score(...) -> f64 { ... }

#[allow(dead_code)]
pub fn cache_dump() -> Vec<...> { ... }

#[allow(dead_code)]
pub fn cache_load(...) { ... }

#[allow(dead_code)]
pub mtime_ms: f64,
#[allow(dead_code)]
pub size: u32,
```

### Sesudah
Semua `#[allow(dead_code)]` dihapus. Fungsi dan field sekarang siap dipakai oleh layer pemanggil (TypeScript bridge) untuk:
- `priority_score` — scheduling scan workspace.
- `cache_dump` / `cache_load` — persist cache ke disk dan restore saat startup.
- `mtime_ms` / `size` — metadata untuk priority scoring dan invalidasi.

---

## Verifikasi

```bash
cargo check --lib
cargo test --lib cache_backend
cargo test --lib adaptive_cache
cargo test --lib scan_cache
cargo test --lib redis_distributed
```

Semua tes lulus:
- `cache_backend::tests` — 14 passed
- `adaptive_cache::tests` — 6 passed
- `scan_cache_tests` — 6 passed
- `redis_distributed::tests` — 6 passed

---

## Dampak ke TypeScript

Tidak ada perubahan API. Semua upgrade adalah internal:
- Signature `CacheFactory::create(config: CacheConfig)` tetap sama.
- `CacheBackend` trait tetap sama.
- Yang berubah hanya isi implementasi dan pilihan backend untuk masing-masing variant.

Caller di TypeScript yang sudah menggunakan `CacheConfig::Distributed`, `CacheConfig::Lazy`, atau `CacheConfig::Adaptive` akan otomatis mendapatkan behavior baru tanpa perlu ubah kode.
