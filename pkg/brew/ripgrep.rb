require  'formula'
class Ripgrep < Formula
  version '0.2.0'
  desc "Search tool like grep and The Silver Searcher."
  homepage "https://github.com/BurntSushi/ripgrep"

  if Hardware::CPU.is_64_bit?
    url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "f55ef5dac04178bcae0d6c5ba2d09690d326e8c7c3f28e561025b04e1ab81d80"
  else
    url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-i686-apple-darwin.tar.gz"
    sha256 "d901d55ccb48c19067f563d42652dfd8642bf50d28a40c0e2a4d3e866857a93b"
  end

  def install
    bin.install "rg"
    man1.install "rg.1"
  end
end
