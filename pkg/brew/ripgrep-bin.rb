class RipgrepBin < Formula
  version '0.2.8'
  desc "Search tool like grep and The Silver Searcher."
  homepage "https://github.com/BurntSushi/ripgrep"
  url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-apple-darwin.tar.gz"
  sha256 "349aba7561028e869932bae8fd27cd5ce45a68f47f05d426d6701a50a8474aa0"

  conflicts_with "ripgrep"

  def install
    bin.install "rg"
    man1.install "rg.1"
  end
end
