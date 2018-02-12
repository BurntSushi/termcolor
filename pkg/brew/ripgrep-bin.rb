class RipgrepBin < Formula
  version '0.8.0'
  desc "Recursively search directories for a regex pattern."
  homepage "https://github.com/BurntSushi/ripgrep"

  if OS.mac?
      url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "fb35e92fd57d28a1e68daf964764c3da7f027ad30cca7a07a1848224776f36b2"
  elsif OS.linux?
      url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "621f2f16f65203aa37e7c10ecfb22384c4c01e70ebbd30a13c0d6944ffc6e59e"
  end

  conflicts_with "ripgrep"

  def install
    bin.install "rg"
    man1.install "doc/rg.1"

    bash_completion.install "complete/rg.bash"
    fish_completion.install "complete/rg.fish"
    zsh_completion.install "complete/_rg"
  end
end
