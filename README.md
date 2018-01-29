# Dilbert Comic Feed Generator ☕

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

The official [Dilbert](http://dilbert.com) Atom feed does not contain embedded
images but instead just provides links to the comics. This makes for a subpar
experience when accessing the feed through a feed reader. This tool remedies the
situation by fetching the official feed and generating a new one with the images
include directly into the content.

### Installation

```
cargo install --git 'https://bitbucket.org/Soft/dilbert-feed.git'
```

