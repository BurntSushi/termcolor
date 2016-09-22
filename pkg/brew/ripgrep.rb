require  'formula'
class Ripgrep < Formula
  version '0.1.14'
  desc "Search tool like grep and The Silver Searcher."
  homepage "https://github.com/BurntSushi/ripgrep"

  if Hardware::CPU.is_64_bit?
    url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "dbbe1099ca6e6381a0585c17718b31e266b0c7426fc0343a096677c3efab9777"
  else
    url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-i686-apple-darwin.tar.gz"
    sha256 "41c2c43a69afddc6205257b38f50f42ad6424d5d7e21720de5cc1bde3849921c"
  end

  def install
    bin.install "rg"
  end
end
