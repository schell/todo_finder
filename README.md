<div align="center">
  <h1>
    <img src="8252.png" /><br />
    todo_finder
  </h1>

  <blockquote>
    If only we had fixed that TODO...
   <br />
   <footer>- Future earth developer, before the great outage</footer>

  </blockquote>
</div>

`todo_finder` finds TODOs in your source code and records them as github issues.


## install

### from source
After cloning this repo and `cd`ing into it, you can install with:
```
bash .ci/common.sh
cargo install --debug --path ./todo_finder --root $HOME/.cargo/
```
This will install the rust toolchain and any dependencies, like `ripgrep`, and
then install the `todo_finder` executable.

### from crates.io
To install from crates.io you'll need a rust toolchain. I prefer to work with
[rustup](https://rustup.rs/).

Then install `ripgrep`, which provides the broadphase filesystem search used by
`todo_finder`:

```bash
cargo install ripgrep
```

then install `todo_finder` run:

```bash
cargo install todo_finder
```


## use

Use `todo_finder` from the command line within the directory you would like
to search. Found TODOs can be dumped to a file or synchronized with the GitHub
Issues of the repository being searched, if the current directory is a git repo.

### Syncing with GitHub Issues

```bash
todo_cli -o github --auth XXX12340981723409872783asonetuhHtonoas24 -l todo
```

The above command would search through the current directory for TODOs and
attempt to publish the results to the repos GitHub issues using the label "todo".
This command requires a github auth token.

### Dumping to a file

```bash
todo_cli -o markdown
```

The above command would dump any found TODOs into a markdown file in the current
directory called `todos.md`.
