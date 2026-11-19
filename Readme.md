# LocalMind Technical Documentation

This document describes the LocalMind command-line and terminal program. It applies ASD-STE100 writing rules. The document does not have formal ASD-STE100 certification.

## 1. Introduction

LocalMind is a local search program. It reads text files in a watch folder and makes a vector for each file. A vector is a list of numbers that represents the meaning of the text. The program ranks the files by their relevance to a query.

LocalMind does the following:

- Reads `.txt`, `.pdf`, and `.docx` files.
- Creates vectors with the `all-MiniLM-L6-v2` embedding model.
- Stores the vectors in a binary file named `index.bin`.
- Combines vector search with keyword search (BM25).
- Shows the results in a terminal user interface (TUI).
- Checks the watch folder every second and updates the index when a file changes.

LocalMind does not use an external database, a cloud service, or an API key. All processing runs on the central processing unit (CPU).

## 2. Requirements

| Item | Requirement |
|---|---|
| Rust toolchain | Stable, 2021 edition or later |
| Network | Required on the first run only, to download the model |
| Terminal | Must support raw mode and an alternate screen |
| Operating system | Any. Opening files from the results list works on Windows only. |

The model files are stored in the Hugging Face cache. They require about 90 MB of storage.

## 3. Build

Run the following commands in the project folder:

1. Build the release binary:

   ```bash
   cargo build --release
   ```

2. Find the binary in `target/release/`. The file name is `localmind` on Linux and macOS, and `localmind.exe` on Windows.

## 4. Run Modes

| Command | Behavior |
|---|---|
| `localmind` | Starts the TUI and watches the watch folder. |
| `localmind --demo` | Runs a scripted test. See section 11 before you use this mode. |

Other arguments are ignored. The program has no other options.

## 5. Watch Folder

LocalMind reads one fixed folder. The path is:

| Operating system | Path |
|---|---|
| Windows | `%TEMP%\localmind_watch\` |
| Linux and macOS | `/tmp/localmind_watch/` |

The program searches subfolders. Place your files in this folder or in a subfolder of it.

The program creates the index file at `localmind_watch/index.bin`. You do not need to create or edit this file.

### 5.1 Supported file types

| Extension | Handling |
|---|---|
| `.txt` | Reads the file as plain text. |
| `.pdf` | Extracts the text layer. A scanned PDF without a text layer has no searchable text. |
| `.docx` | Reads `word/document.xml` and removes the XML tags. Formatting, tables, headers, and images are not read. |
| `.doc` | Found by the scanner, but not read. The file is treated as an empty file. Convert the file to `.docx` or `.txt`. |
| `.md` | Not in the supported list. To add this type, see section 13. |

The program ignores a file in these cases:

- The file name starts with `.`.
- The file name ends with `.tmp` or `.swp`.
- The extension is not in the supported list.

## 6. Index File Format

The file `index.bin` stores all vectors and paths. The program writes this file after each indexing run.

The file has four parts, in this order:

| Part | Size | Content |
|---|---|---|
| Header | 16 bytes | Magic value `LMDB`, version (`u32`), record count (`u32`), vector dimension (`u32`). |
| Vectors | `n × d × 4` bytes | One `f32` vector for each record. The block starts on an 8-byte boundary. |
| Entry table | `n × 16` bytes | One entry for each record. Each entry holds a byte offset (`u64`), a length (`u32`), and 4 bytes of padding. |
| Strings | Variable | The path text of all records, joined without a separator. |

In this version, `d` is 384 and `n` is the number of files indexed.

The program reads the file with a memory map. The operating system loads only the parts that the program uses.

Warning: the program does not check the file length when it opens the file. A damaged file can cause a crash. If this happens, delete `index.bin`. The program creates a new file at the next start.

## 7. Terminal User Interface

### 7.1 Screen layout

```
┌ Search ─────────────────────────────────────────────┐
│ > your query                                         │
├ Results ──────────────┬ <file name> ────────────────┤
│  1. 0.033 /path/a.txt │ file text, up to 500 lines  │
│  2. 0.031 /path/b.docx│ query terms highlighted     │
├───────────────────────┴─────────────────────────────┤
│ 10 results (0.02s)                                   │
└─────────────────────────────────────────────────────┘
```

| Area | Content |
|---|---|
| Search box | The query text. |
| Results list | Up to 10 files, in order of relevance. Each line shows the rank, the score, and the path. |
| Preview | The text of the selected file. The program shows up to 500 lines. |
| Status bar | The current state of indexing or searching. |

### 7.2 Keyboard controls

| Key | Action |
|---|---|
| Any character | Adds the character to the query. A search starts 200 ms after the last key press. |
| `Backspace` | Deletes the character before the cursor. |
| `Delete` | Deletes the character after the cursor. |
| `←` and `→` | Moves the cursor. |
| `Home` and `End` | Moves the cursor to the start or end of the query. |
| `↑` and `↓` | Selects the previous or next result. |
| `Enter` | Opens the selected file. Works on Windows only. |
| `Esc` | Clears the query. If the query is empty, the program quits. |
| `Ctrl+C` | Quits the program. |

Note: a search does not need the `Enter` key. The search runs automatically.

### 7.3 Status messages

| Message | Meaning |
|---|---|
| `Type a query to search` | The program is ready. The query is empty. |
| `Indexing N/M` | The program is embedding file N of M. |
| `Searching...` | A search is in progress. |
| `K results (T s)` | The search found K files in T seconds. |

### 7.4 Scores

The results list shows a score for each file. In this version, the score is the Reciprocal Rank Fusion (RRF) value. It is not a cosine similarity. RRF values are small, usually below 0.04. Therefore, the color is gray for almost all results. Use the score only to compare results within the same query.

## 8. How Search Works

### 8.1 Vector search

1. The program converts the query to a vector with the same model.
2. The program computes the cosine similarity between the query vector and each stored vector.
3. The program sorts the results by similarity.

Cosine similarity has a value from -1 to 1. A higher value means that the meanings are closer.

### 8.2 Keyword search (BM25)

BM25 finds files that contain the query words. The program uses this tokenizer:

- Splits the text at any character that is not a letter, a digit, `-`, or `_`.
- Converts the tokens to lower case.
- Keeps only tokens with at least two characters.

BM25 uses these constants: `k1 = 1.2` and `b = 0.75`.

### 8.3 Hybrid ranking

The program combines the two rankings with Reciprocal Rank Fusion:

```
RRF(file) = 1 / (60 + rank_vector(file)) + 1 / (60 + rank_bm25(file))
```

A file that is not in one ranking has no value from that ranking. The program keeps the 10 files with the highest RRF values.

The program does not use a minimum score. A search returns results even when no file is relevant. The first results can be unrelated to the query.

## 9. Embedding

### 9.1 Model

The model is `sentence-transformers/all-MiniLM-L6-v2`. It produces a vector of 384 values for each input.

### 9.2 Input length

| Text length | Method |
|---|---|
| Up to 10,000 characters | The program embeds the text in one step. The model reads only the first 256 tokens. The rest of the text is not used. |
| More than 10,000 characters | The program splits the text into overlapping parts of up to 256 tokens. It averages the part vectors into one vector for the file. |

Note: an average of many parts can represent several topics as one vague meaning.

### 9.3 Startup behavior

At each start, the program embeds every file in the watch folder. The program does not load the previous `index.bin`. Startup time increases with the number of files.

## 10. Live Updates

A background thread checks the watch folder once each second. The thread does the following:

1. Computes a BLAKE3 hash of the content of each file.
2. Compares the hashes with the records in memory.
3. If a file changed, adds, or deletes, the thread updates the index.

| Change | Result |
|---|---|
| New file | The program extracts the text, embeds it, and adds a record. |
| Changed content | The program extracts the text and embeds it again. |
| Deleted file | The program removes the record. |
| Changed date only | No action. The hash is the same. |

After a change, the program rebuilds the BM25 index and writes a new `index.bin`. The new file is written to `index.tmp` and then renamed to `index.bin`.

## 11. Demo Mode

Run the demo with this command:

```bash
localmind --demo
```

The demo does the following:

1. Embeds a sample sentence and shows the vector size and the time.
2. Writes and reads a small test index.
3. Runs a vector search on three test vectors.
4. Runs an end-to-end search on eight short sentences.
5. Creates, changes, and deletes files in the watch folder.
6. Checks Unicode text, a batch of five files, and a long file.

Warning: the demo deletes the whole `localmind_watch` folder at the start and at the end. Move your files out of this folder before you run the demo.

The demo can take several minutes, because the CPU embeds each file slowly.

## 12. Troubleshooting

| Symptom | Cause and action |
|---|---|
| The program stops at the start with a Hugging Face error. | There is no network connection. Connect to the network and start the program again. The model is cached after the first download. |
| A file does not appear in the results. | Check the extension and the file name. Wait up to two seconds for the next check. |
| A `.doc` file has no results. | Convert the file to `.docx` or `.txt`. |
| A `.pdf` or `.docx` preview shows `[error reading file]`. | The preview reads plain text only. The search still uses the extracted text. |
| The `Enter` key does nothing. | The program runs on a system other than Windows. Open the file path with another program. |
| The results do not match the query. | The model ranks files even when no file is relevant. Use more specific words. |
| All results are gray. | This is expected. The displayed values are RRF scores. |
| The program stops after a damaged index. | Delete `localmind_watch/index.bin`. The program creates a new file at the next start. |

## 13. Known Limitations

- The watch folder is fixed. The program has no option to select another folder.
- The program has no command-line option other than `--demo`. To change the result count, the model, or the folder, you must change the source code.
- The program always returns up to 10 results when the index has records. It has no minimum score.
- The model reads only the first 256 tokens of a short file.
- The program does not read `.md` files in this version. To add this type, add `md` to the list in the function `collect_files` in `src/monitor.rs`:

  ```rust
  if matches!(ext.as_str(), "txt" | "md" | "pdf" | "docx" | "doc") {
  ```

  Convert the extension to lower case first, as shown in the source code.

- The program opens files only on Windows.
- The program embeds all files at each start. It does not reuse the saved index.
- The index update replaces the file in use by the memory map. On Windows, the rename can fail. The program ignores this error. In that case, the vector index is not updated while the BM25 index is updated. Restart the program after you change many files.
- A file that fails extraction is stored with a zero vector at start. It is not stored at all after a live change.
- The polling check uses about one percent of one CPU core.
- The text preview does not show extracted text for PDF or DOCX files.

## 14. Project Structure

| File | Purpose |
|---|---|
| `src/main.rs` | Starts the program. Runs the demo when you use `--demo`. |
| `src/embed.rs` | Loads the model and the tokenizer. Creates the vectors. |
| `src/extract.rs` | Reads the text of `.txt`, `.pdf`, and `.docx` files. |
| `src/index.rs` | Saves and opens `index.bin`. |
| `src/bm25.rs` | Builds the keyword index and scores the BM25 ranking. |
| `src/search.rs` | Runs vector search and hybrid search with RRF. |
| `src/monitor.rs` | Checks the watch folder once each second and updates the index. |
| `src/tui.rs` | Shows the terminal user interface. |

## 15. Glossary

| Term | Definition |
|---|---|
| Embedding | A list of numbers that represents the meaning of a text. |
| Vector | A list of numbers. In this program, each vector has 384 values. |
| Cosine similarity | A value that shows how close two vectors are in direction. |
| BM25 | A ranking method that scores files by the words they contain. |
| RRF | Reciprocal Rank Fusion. A method that combines several rankings by their position. |
| Memory map | A method that lets the program read a file as if it were in memory. |
| Hash | A short value that identifies the content of a file. This program uses BLAKE3. |
| Token | A small unit of text, such as a word or part of a word. |
