# Pinned Upstream Revisions

The repository is designed to track its large upstream trees as Git
submodules. Results are reproducible only when these revisions are recorded:

| Component | Revision | Purpose |
| --- | --- | --- |
| Boa | `de2221a09c132951c2ebad36e62ecd20b9987215` | Current ECMAScript backend |
| QuickJS | `04be246001599f5995fa2f2d8c91a0f198d3f34c` | Performance/design reference |
| Test262 | `de8e621cdba4f40cff3cf244e6cfb8cb48746b4a` | Conformance corpus |
| JetStream 2.0 | `60cdba17bef0dcdb3fca2263e3916c3c45bfb7c2` | Performance benchmark |

After cloning, initialize all dependencies with:

```sh
git submodule update --init --recursive
```

When updating a component, record the new revision in benchmark and
conformance reports. Do not update all three references in the same change
unless the compatibility impact has been measured.
