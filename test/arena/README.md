# Test Arena - Data Generator

## Purpose

This directory contains a test data generator (`gen.py`) for the bitslides project. The script creates a test environment with fake file structures and content to test the bitslides synchronization functionality.

## Files

- `gen.py` - Test data generator script
- `test.conf` - Configuration file for testing

## What the Generator Creates

### Test Volumes
The script generates test data for three volumes:
- **Laptop**
- **Server**
- **Pendrive**

### Folder Structure
For each volume, it creates nested directories under `Slides/[volume]/[folder_kind]` where `folder_kind` can be:
- `audio` - Audio files
- `video` - Video files
- `image` - Image files
- `office` - Office documents
- `text` - Text files

### Generated Files
- **Random filenames** appropriate to their category (using Faker library)
- **Random text content** between 1-10 KB per file
- **SHA256 checksum files** (`.sha256`) for each generated file to verify data integrity

### Output Directories

The generator creates three directories:

1. **`original/`** - Initial file structure with all generated content
2. **`processed/`** - Copy of original directory (used for testing sync operations)
3. **`expected/`** - Expected state after synchronization completes

## Usage

All commands should be run from the repository root directory.

### Step 1: Install Requirements

```bash
pip install faker
```

### Step 2: Generate Test Data

Run the generator script:

```bash
python3 test/arena/gen.py
```

This will create three directories in `test/arena/`:
- `test/arena/original/` - Contains the initial test file structure
- `test/arena/processed/` - A copy of original with `test.conf` included (working directory for testing)
- `test/arena/expected/` - The expected result after synchronization

### Step 3: Test bitslides

Once the test data is generated, you can test bitslides synchronization:

```bash
# Run bitslides with the test configuration
cargo run -- --config test/arena/processed/test.conf
```

### Step 4: Verify Results

After running bitslides, compare the `processed/` directory with the `expected/` directory to verify that the synchronization worked correctly:

```bash
# Compare directories
diff -r test/arena/processed/ test/arena/expected/

# Or use tree to visualize the structure
tree test/arena/processed/
tree test/arena/expected/
```

### Regenerating Test Data

To start fresh, simply run the generator again. The script automatically cleans up existing directories before generating new test data:

```bash
python3 test/arena/gen.py
```

### Requirements
- Python 3
- faker library: `pip install faker`

## Implementation Details

- Generates between 1-5 folders per volume
- Generates between 1-10 files per folder
- Each file gets an accompanying `.sha256` checksum file for verification

## Notes

The script automatically cleans up any existing `original/`, `processed/`, and `expected/` directories before generating new test data.
