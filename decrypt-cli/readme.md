# Build
`cargo build --release`

# How to use
```
> ../target/release/decrypt-cli -h
Usage: decrypt-cli [OPTIONS] --pwd <password> --dir <PATH>

Options:
      --pwd <password>  graph password
      --dir <PATH>      graph data dir path
      --dst <PATH>      dir to store decrypted data [default: ./decrypted]
  -h, --help            Print help
```

```
> ../target/release/decrypt-cli --pwd <graph-password> --dir <path-to-encrypted-graph-dir>
keys.edn: Keys { encrypted_secret_key: "-----BEGIN AGE ENCRYPTED FILE-----\nYWdlLWVuY3J5cHRpb24ub3JnL3YxCi0+IHNjcnlwdCBaRmJkNmZzUGtlbnE0S04v\nMzhCaXBBIDIwCkw0K3hZRm95cU9kdktwMTZQSGNOL3J0VTBsaWx0U2JCQTl1UFM0\nWlR4cDAKLS0tIHdaWkZ1M3kxa1BnSDRHK0JIZkRMcDMxS3RwSmhhczFRbGVWV3pV\nUHRKeXcK7qCa3dYt76DU2yeXGujkN1mMIoDIBl4XhVf2Q8x1UnftPuXMqP8Y9+2D\n21l1sd1iW6SdsNavtwFQXIqFUOuR/d0ztPx3Zkv8/rflpDPxkwweIX5FBAxx+r4A\nxNfNY1Rz5lRjgLRURR8vFA==\n-----END AGE ENCRYPTED FILE-----\n", _public_key: "age1gf0ez92csj597krdv05sgu7ge63j69lyt0d9m3tq55y96l5cyvrqcm6ysd" }
secret key: AGE-SECRET-KEY-1HZVUHUUAPJEFAT46W67WC5QQHVDXQCD7UVRHJSR0DS2P3NEFJ4VSKVNTAG
dst dir: "./decrypted"
Generated "./decrypted/logseq/custom.css"
Generated "./decrypted/logseq/config.edn"
Generated "./decrypted/journals/2024_08_02.md"
Generated "./decrypted/pages/contents.md"
```
