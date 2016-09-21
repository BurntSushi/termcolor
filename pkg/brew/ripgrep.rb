require  'formula'
class Ripgrep < Formula
  version '0.1.8'
  desc "Search tool like grep and The Silver Searcher."
  homepage "https://github.com/BurntSushi/ripgrep"

  if Hardware::CPU.is_64_bit?
    url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "893e0e7fac88ebbef024829466fafef6eae5b1060273bbfca3806090e660b06b"
  else
    url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-i686-apple-darwin.tar.gz"
    sha256 "2296c8081a2bfe28b43dea4326a9e8ce9c2821fd628a1ca366e824aceddc5fad"
  end

  def install
    bin.install "rg"
  end
end
