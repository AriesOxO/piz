# Homebrew formula for piz
# This file is a template. The release workflow automatically generates
# the actual formula with correct version and SHA256 checksums,
# and pushes it to the AriesOxO/homebrew-tap repository.
#
# Usage:
#   brew tap AriesOxO/tap
#   brew install piz
# Or:
#   brew install AriesOxO/tap/piz

class Piz < Formula
  desc "Intelligent terminal command assistant - translate natural language to shell commands"
  homepage "https://github.com/AriesOxO/piz"
  version "${VERSION}"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/AriesOxO/piz/releases/download/v#{version}/piz-aarch64-apple-darwin.tar.gz"
      sha256 "${SHA256_MACOS_ARM64}"
    else
      url "https://github.com/AriesOxO/piz/releases/download/v#{version}/piz-x86_64-apple-darwin.tar.gz"
      sha256 "${SHA256_MACOS_X86_64}"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/AriesOxO/piz/releases/download/v#{version}/piz-aarch64-linux-musl.tar.gz"
      sha256 "${SHA256_LINUX_ARM64}"
    else
      url "https://github.com/AriesOxO/piz/releases/download/v#{version}/piz-x86_64-linux-musl.tar.gz"
      sha256 "${SHA256_LINUX_X86_64}"
    end
  end

  def install
    bin.install "piz"
  end

  def caveats
    <<~EOS
      To enable shell integration (cd/export/source support), add to your shell config:

        # bash (~/.bashrc)
        eval "$(piz init bash)"

        # zsh (~/.zshrc)
        eval "$(piz init zsh)"

        # fish (~/.config/fish/config.fish)
        piz init fish | source

      Shell integration also provides convenient aliases:
        p  → piz
        pf → piz fix
        pc → piz chat
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/piz --version")
  end
end
