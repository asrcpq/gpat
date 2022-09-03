# gpat

Convert and sync linear git repository to patches(and back).

For corruption-resistant source code storage on my append-only filesystem.

# usage

```
sync A.gpat B.git
sync A.git B.gpat
```

# notes

Does not preserve all metadata
like **commit message**, author, mail address, time zone.
