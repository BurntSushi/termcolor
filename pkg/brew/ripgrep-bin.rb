class RipgrepBin < Formula
  version '0.2.3'
  desc "Search tool like grep and The Silver Searcher."
  homepage "https://github.com/BurntSushi/ripgrep"
  url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-apple-darwin.tar.gz"
  sha256 "7407555dfe040a2631a7efdd1eea62cf1d1c50e5a6ecf8ee82e0bef9d5f37298"

  conflicts_with "ripgrep"

  def install
    bin.install "rg"
    man1.install "rg.1"
  end
end
