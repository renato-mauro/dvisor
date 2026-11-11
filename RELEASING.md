# Releasing

dvisor is published to npm as [`dvisor`](https://www.npmjs.com/package/dvisor).
jsDelivr and unpkg serve every published version from there, so there is
nothing else to upload.

1. Build the engine and the library: `cd dvisor-back && ./compile.sh`.
2. Set the new version in `dvisor-web-lib` with
   `npm version X.Y.Z --no-git-tag-version`, and the same version in
   `dvisor-back/Cargo.toml`. A version number, once published, can never be
   used again, even after an unpublish.
3. Commit, tag `vX.Y.Z` and push.
4. In `dvisor-web-lib`: `npm publish`. It runs the whole test suite first
   (`prepublishOnly`).
5. In `dvisor-web-lib`: `npm run check:cdn`. It loads the new version from
   jsDelivr and unpkg, as a site would, and runs a query.

Versions before 1.0.0 were served from
`https://storage.googleapis.com/dvisor/lib/<version>/`, and stay there.
