# Test Arena

Test data generator for bitslides. Creates a test environment with fake file structures to test synchronization functionality.

## Requirements

- Python 3
- faker library: `pip install faker`

## Files

- `gen.py` - Generates test data
- `test.conf` - Configuration for testing

## Generated Test Data

### Volumes
- Laptop
- Server
- Pendrive

### Content
- 1-5 folders per volume (audio, video, image, office, text)
- 1-10 files per folder with random names and content (1-10 KB)
- SHA256 checksum files (`.sha256`) for each file

### Directories Created
- `test/arena/original/` - Initial file structure
- `test/arena/processed/` - Working copy for sync operations
- `test/arena/expected/` - Expected state after sync

## Usage

Run all commands from the repository root.

### 1. Generate test data

```bash
python3 test/arena/gen.py
```

### 2. Start bitslides

```bash
cargo run -- --config test/arena/processed/test.conf -vv &
```

### 3. Verify sync

```bash
diff -r test/arena/processed/ test/arena/expected/
```

### 4. Test live sync

```bash
echo "migrating" > test/arena/processed/Laptop/Slides/Pendrive/text/move.me
sleep 2
cat test/arena/processed/Pendrive/Slides/Pendrive/text/move.me
```

The file should be moved from Laptop to Pendrive volume.

### 5. Stop bitslides

```bash
kill %1
```

### Regenerate

```bash
python3 test/arena/gen.py
```
