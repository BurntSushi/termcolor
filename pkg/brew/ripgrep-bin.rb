class RipgrepBin < Formula
  version '0.5.2'
  desc "Search tool like grep and The Silver Searcher."
  homepage "https://github.com/BurntSushi/ripgrep"
  url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-apple-darwin.tar.gz"
  sha256 "a0326a84af8517ad707d8c7cccba6e112de27822c391cc0937e4727fbb6c48f4"

  conflicts_with "ripgrep"

  def install
    bin.install "rg"
    man1.install "rg.1"

    bash_completion.install "complete/rg.bash-completion"
    fish_completion.install "complete/rg.fish"
    zsh_completion.install "complete/_rg"
  end
end
