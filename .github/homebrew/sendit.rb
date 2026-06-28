# Template for the getkono/homebrew-tap formula. The Release-plz workflow's
# "update-tap" job fills in the version and four SHA-256 values below on each
# release, then commits the rendered file to the tap. The capitalised tokens
# are substituted automatically -- leave them intact when editing. Lint with:
#   ruby -c .github/homebrew/sendit.rb
#   brew style .github/homebrew/sendit.rb
class sendit < Formula
  desc "High-level library for creating PRs (e.g. GitHub) with Rust."
  homepage "https://github.com/getkono/sendit"
  version "__VERSION__"
  license "MIT or APACHE-2.0"

  on_macos do
    on_arm do
      url "https://github.com/getkono/sendit/releases/download/v#{version}/sendit-aarch64-apple-darwin.tar.gz"
      sha256 "__SHA256_AARCH64_APPLE_DARWIN__"
    end
    on_intel do
      url "https://github.com/getkono/sendit/releases/download/v#{version}/sendit-x86_64-apple-darwin.tar.gz"
      sha256 "__SHA256_X86_64_APPLE_DARWIN__"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/getkono/sendit/releases/download/v#{version}/sendit-aarch64-unknown-linux-musl.tar.gz"
      sha256 "__SHA256_AARCH64_UNKNOWN_LINUX_MUSL__"
    end
    on_intel do
      url "https://github.com/getkono/sendit/releases/download/v#{version}/sendit-x86_64-unknown-linux-musl.tar.gz"
      sha256 "__SHA256_X86_64_UNKNOWN_LINUX_MUSL__"
    end
  end

  def install
    bin.install "sendit"
  end

end
