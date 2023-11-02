# The sync protocol

```clojure
(defprotocol IRSAPI
  (get-local-files-meta [this graph-uuid base-path filepaths] "get local files' metadata")
  (get-local-all-files-meta [this graph-uuid base-path] "get all local files' metadata")
  (rename-local-file [this graph-uuid base-path from to access-token])
  (update-local-file [this graph-uuid base-path filepath access-token] "remote -> local")
  (delete-local-file [this graph-uuid base-path filepath access-token])
  (update-remote-file [this graph-uuid base-path filepath local-txid access-token] "local -> remote")
  (delete-remote-file [this graph-uuid base-path filepath local-txid access-token]))
```
