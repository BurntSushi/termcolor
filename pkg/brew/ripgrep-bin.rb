class RipgrepBin < Formula
  version '0.3.2'
  desc "Search tool like grep and The Silver Searcher."
  homepage "https://github.com/BurntSushi/ripgrep"
  url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-apple-darwin.tar.gz"
  sha256 "05869abe67104822d29081f12e31e3e90c29cac60ee50546387b17e9be45739c"

  conflicts_with "ripgrep"

  def install
    bin.install "rg"
    man1.install "rg.1"
  end
end
