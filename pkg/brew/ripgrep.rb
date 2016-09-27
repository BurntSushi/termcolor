require  'formula'
class Ripgrep < Formula
  version '0.2.1'
  desc "Search tool like grep and The Silver Searcher."
  homepage "https://github.com/BurntSushi/ripgrep"

  if Hardware::CPU.is_64_bit?
    url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "f8b208239b988708da2e58f848a75bf70ad144e201b3ed99cd323cc5a699625f"
  else
    url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-i686-apple-darwin.tar.gz"
    sha256 "3880ffbc169ea7a884d6c803f3b227a9a3acafff160cdaf830f930e065ae2b38"
  end

  def install
    bin.install "rg"
    man1.install "rg.1"
  end
end
