# Design

1. **CSV, not xlsx.** Every spreadsheet exports CSV; parsing xlsx pulls a large dependency
   into the service for no semantic gain. The admin UI can offer an xlsx-to-CSV step client
   side if needed.
2. **Synchronous up to 10000 rows**, the same bound as generation; a job queue with a log
   file is the promocode-service shape, not the requirement. Beyond that bound the caller
   splits the file.
3. **Skip, do not fail, on `code_exists`**: imports are re-run after partial failures; the
   report makes the outcome explicit.
4. **Export carries derived fields** (`uses`, `state`) so marketing sees what is left, and
   the header equals the import header so a file can be edited and re-imported.
