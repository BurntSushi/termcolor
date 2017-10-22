class RipgrepBin < Formula
  version '0.7.1'
  desc "Search tool like grep and The Silver Searcher."
  homepage "https://github.com/BurntSushi/ripgrep"
  url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-apple-darwin.tar.gz"
  sha256 "ee670b0fba46323ee9a2d1c5b8bee46fa3e45778f6f105f2e8e9ee29e8bd0d45"

  conflicts_with "ripgrep"

  def install
    bin.install "rg"
    man1.install "rg.1"

    bash_completion.install "complete/rg.bash-completion"
    fish_completion.install "complete/rg.fish"
    zsh_completion.install "complete/_rg"
  end
end
