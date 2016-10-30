class RipgrepBin < Formula
  version '0.2.5'
  desc "Search tool like grep and The Silver Searcher."
  homepage "https://github.com/BurntSushi/ripgrep"
  url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-apple-darwin.tar.gz"
  sha256 "c6775a50c6f769de2ee66892a700961ec60a85219aa414ef6880dfcc17bf2467"

  conflicts_with "ripgrep"

  def install
    bin.install "rg"
    man1.install "rg.1"
  end
end
