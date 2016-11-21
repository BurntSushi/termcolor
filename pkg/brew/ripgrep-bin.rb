class RipgrepBin < Formula
  version '0.3.0'
  desc "Search tool like grep and The Silver Searcher."
  homepage "https://github.com/BurntSushi/ripgrep"
  url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-apple-darwin.tar.gz"
  sha256 "a177195e31a6687e1b0141cbb93bb2fc915a49c4bca26d7094a8144ebdfb3a69"

  conflicts_with "ripgrep"

  def install
    bin.install "rg"
    man1.install "rg.1"
  end
end
