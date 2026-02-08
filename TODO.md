# 🔧 Refactoring Suggestions for Bitslides

## TOC
### Critical (Do First):
1. Remove all production `unwrap()` calls
2. Fix error message quality
3. Implement proper types for SyncJobs (remove FIXME)
4. Make collision/check policies configurable

### High Priority:
5. ✅ Create a workspace where you have 1 lib and 1 bin
5. ✅ Extract Tracer into its own module
6. Fix WIP file naming
7. Add timeouts to file operations

### Medium Priority:
8. Better async handling with `try_join_all`
9. Add module-level documentation
10. Create constants for magic numbers

### Nice to Have:
11. FileSystem trait for testability
12. Separate planning from execution
13. Complete or remove `tidy_up()`

---

## **1. High Priority - Error Handling & Safety**

### **1.1 Remove `.unwrap()` calls in production code**
Replace panicking `unwrap()` calls with proper error handling:

**In `mod.rs` line 235, 239, 295, 298, 370:**
```rust
// Current:
let entry_name = entry_fullpath.file_name().unwrap().to_string_lossy().to_string();

// Better:
let entry_name = entry_fullpath
    .file_name()
    .ok_or_else(|| anyhow!("Invalid path: no filename"))?
    .to_string_lossy()
    .to_string();
```

**In `main.rs` line 33, 56:**
```rust
// Current:
let trace_parent = trace.parent().unwrap();

// Better:
let trace_parent = trace.parent()
    .ok_or_else(|| anyhow!("Trace path has no parent: {trace:?}"))?;
```

### **1.2 Better error messages**
```rust
// Current (line 112 mod.rs):
Err(_) => log::warn!("Error processing some volumes"),

// Better:
Err(e) => log::warn!("Error processing volumes in rootset {}: {}", rootset_config.keyword, e),
```

---

## **2. Code Organization & Module Structure**

### **2.1 Extract Tracer into its own module** -> DONE

### **2.2 Create proper types for SyncJobs**
**In `syncjob.rs`:** (line 61 has FIXME)
```rust
pub struct SyncJobs {
    inner: Vec<SyncJob>,
}

impl SyncJobs {
    pub fn new(jobs: Vec<SyncJob>) -> Self {
        Self { inner: jobs }
    }

    pub fn sort_by_priority(&mut self) {
        // Sort by: direct routes first, then indirect
        self.inner.sort_by_key(|job| {
            if job.via == job.dst { 0 } else { 1 }
        });
    }

    pub fn into_iter(self) -> impl Iterator<Item = SyncJob> {
        self.inner.into_iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut SyncJob> {
        self.inner.iter_mut()
    }
}
```

---

## **3. Type Safety & API Improvements**

### **3.1 Use typed paths consistently**
Create a type for volume paths:
```rust
pub struct VolumePath(PathBuf);

impl VolumePath {
    pub fn slides_dir(&self, keyword: &str) -> PathBuf {
        self.0.join(keyword)
    }

    pub fn slide_path(&self, keyword: &str, slide_name: &str) -> PathBuf {
        self.slides_dir(keyword).join(slide_name)
    }
}
```

### **3.2 Make the SyncJob trigger handling safer**
```rust
// Current: take_trigger() can panic with "No trigger found"
// Better:
pub fn take_trigger(&mut self) -> Result<tokio::sync::mpsc::Sender<()>> {
    self.inner.tx.take()
        .ok_or_else(|| anyhow!("Trigger already taken for syncjob: {:?}", self))
}
```

---

## **4. Async & Performance**

### **4.1 Use `try_join_all` instead of sequential awaits**
```rust
// In enough() function:
// Current:
for handle in handles {
    let _ = handle.await?;
}

// Better:
use futures::future::try_join_all;
try_join_all(handles).await?;
```

### **4.2 Add timeout to file operations**
```rust
use tokio::time::{timeout, Duration};

// In move_file:
timeout(Duration::from_secs(300), tokio::fs::copy(src_file, wip))
    .await
    .map_err(|_| anyhow!("Copy operation timed out"))?
    ?;
```

---

## **5. Configuration & Constants**

### **5.1 Move magic numbers to constants**
```rust
// In mod.rs line 149:
const DEFAULT_RETRIES: u8 = 5;
const DEFAULT_SAFE_MODE: bool = false;

// In syncjob.rs line 25:
const SYNC_TRIGGER_CHANNEL_SIZE: usize = 1000;
```

### **5.2 Make collision/check policies configurable**
Lines 143-145 in main.rs are marked as FIXME - they should come from config files.

---

## **6. Better Abstraction & Testability**

### **6.1 Extract filesystem operations behind a trait**
```rust
#[async_trait]
pub trait FileSystem {
    async fn copy(&self, src: &Path, dst: &Path) -> Result<u64>;
    async fn rename(&self, src: &Path, dst: &Path) -> Result<()>;
    async fn remove_file(&self, path: &Path) -> Result<()>;
    // ... etc
}

pub struct RealFileSystem;
pub struct MockFileSystem { /* for tests */ }
```

This makes testing much easier without creating temp directories.

### **6.2 Separate business logic from I/O**
Create a `SyncPlanner` that returns a plan, then `SyncExecutor` that executes it:
```rust
pub struct SyncPlan {
    pub operations: Vec<SyncOperation>,
}

pub enum SyncOperation {
    CreateDir(PathBuf),
    MoveFile { src: PathBuf, dst: PathBuf },
    DeleteDir(PathBuf),
}
```

---

## **7. Documentation & Comments**

### **7.1 Add module-level documentation**
```rust
//! # Bitslides Core Library
//!
//! This library implements automatic file synchronization between "volumes" using "slides".
//!
//! ## Concepts
//! - **Volume**: A storage location containing a slides directory
//! - **Slide**: A destination folder within a volume's slides directory
//! - **SyncJob**: A task that syncs files from one slide to another
```

### **7.2 Document the synchronization algorithm**
The complex logic in `build_syncjobs()` needs better documentation.

---

## **8. Specific Issues**

### **8.1 Fix the WIP file naming**
Line 251 in fs.rs has FIXME: "If the file is photo.jpg the wip needs to be .photo.jpg.wip"
```rust
let wip = if request.safe {
    let file_name = dst_file.file_name()
        .ok_or_else(|| anyhow!("Invalid destination path"))?;
    let hidden_name = format!(".{}.wip", file_name.to_string_lossy());
    dst_file.with_file_name(hidden_name)
} else {
    dst_file
};
```

### **8.2 Handle the unused variable warnings**
The `Token` struct fields should be used or marked appropriately.

### **8.3 Complete the TODO items**
- Line 421: "Measure the next block" - add timing/metrics
- Line 488: "Instead of directly synching, create a watcher" - already done?
- Line 167: Implement `tidy_up()` or remove it

---

## **9. Testing Improvements**

### **9.1 Reduce test `unwrap()` usage**
Use `?` operator in tests that return `Result`:
```rust
#[test]
fn test_identify_volumes() -> Result<()> {
    let ctx = setup()?;
    let volumes = identify_volumes(&ctx.roots[0], "slides")?;
    assert_eq!(volumes.len(), 3);
    Ok(())
}
```
