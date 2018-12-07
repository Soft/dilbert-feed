# Dilbert Comic Feed Generator ☕

[![Build Status](https://api.travis-ci.org/Soft/dilbert-feed.svg?branch=master)](https://travis-ci.org/Soft/dilbert-feed)
[![GitHub release](https://img.shields.io/github/release/Soft/dilbert-feed.svg)](https://github.com/Soft/dilbert-feed/releases)
[![dependency status](https://deps.rs/repo/github/soft/dilbert-feed/status.svg)](https://deps.rs/repo/github/soft/dilbert-feed)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

The official [Dilbert](http://dilbert.com) Atom feed does not contain embedded
images but instead just provides links to the comics. This makes for a subpar
experience when accessing the feed through a feed reader. This tool remedies the
situation by fetching the official feed and generating a new one with the images
include directly into the content.

### Installation

Statically linked release binaries available on the [GitHub releases
page.](https://github.com/Soft/dilbert-feed/releases) These binaries should work
on most recent Linux systems without any additional dependencies or
configuration.

Alternatively, `dilbert-feed` can be installed from source using
[Cargo](https://doc.rust-lang.org/stable/cargo/):

```shell
cargo install --git 'https://bitbucket.org/Soft/dilbert-feed.git'
```

