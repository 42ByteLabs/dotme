<!-- markdownlint-disable -->
<div align="center">
<h1>DotMe</h1>

[![GitHub](https://img.shields.io/badge/github-%23121011.svg?style=for-the-badge&logo=github&logoColor=white)][github]
[![Crates.io Version](https://img.shields.io/crates/v/dotme?style=for-the-badge)][crates-io]
[![Crates.io Downloads (recent)](https://img.shields.io/crates/dr/dotme?style=for-the-badge)][crates-io]
[![GitHub Stars](https://img.shields.io/github/stars/42ByteLabs/dotme?style=for-the-badge)][github]
[![Licence](https://img.shields.io/github/license/42ByteLabs/dotme?style=for-the-badge)][license]

</div>
<!-- markdownlint-restore -->

A modern, simple and efficient dotfiles manager written in Rust.

DotMe allows you to easily manage your dotfiles and keep them synchronized across your system.
It supports individual files, directories, and git repositories as dotfile sources.

## Features

- 📁 Manage files, directories, and git repositories
- 🚀 Fast and async - built with Tokio
- 🔧 Simple YAML configuration
- 🎯 Automatic source type detection
- 💻 Clean CLI interface

## 📦 Installation

### Using Cargo

```bash
cargo install dotme
```

## 🚀 Quick Start

### Initialize DotMe

```bash
dotme init
```

### Add dotfiles

**Add a git repository (stored in ~/.dotme/git):**

```bash
dotme add https://github.com/user/dotfiles.git
```

### Update/sync dotfiles

Actually perform the update:

```bash
dotme update
```

### Remove dotfiles

```bash
# Remove with explicit source
dotme remove ~/.bashrc

# Interactive removal (select from list)
dotme remove
```

## 🦸 Support

Please create [GitHub Issues][github-issues] if there are bugs or feature requests.

This project uses [Semantic Versioning (v2)][semver] and with major releases, breaking changes will occur.

## 📓 License

This project is licensed under the terms of the MIT open source license.
Please refer to [MIT][license] for the full terms.

<!-- Resources -->
[license]: ./LICENSE
[semver]: https://semver.org/
[github]: https://github.com/42ByteLabs/dotme
[github-issues]: https://github.com/42ByteLabs/dotme/issues
[crates-io]: https://crates.io/crates/dotme