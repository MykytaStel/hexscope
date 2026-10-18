// The Homebrew formula for a released version, from the checksums the
// release carries (release.yml): the command for macOS and Linux, Apple
// silicon and Intel, as built for that release.
//
//   node scripts/homebrew-formula.mjs 0.7.0 > ../homebrew-hexscope/Formula/hexscope.rb
//
// The tap is github.com/MykytaStel/homebrew-hexscope:
//   brew install mykytastel/hexscope/hexscope
const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) {
  console.error("usage: node scripts/homebrew-formula.mjs 0.7.0");
  process.exit(2);
}

const base = `https://github.com/MykytaStel/hexscope/releases/download/v${version}`;
const TARGETS = {
  macos: { arm: "aarch64-apple-darwin", intel: "x86_64-apple-darwin" },
  linux: { arm: "aarch64-unknown-linux-musl", intel: "x86_64-unknown-linux-musl" },
};

async function sha256(target) {
  const name = `hexscope-${version}-${target}.tar.gz`;
  const res = await fetch(`${base}/${name}.sha256`);
  if (!res.ok) throw new Error(`${name}.sha256: HTTP ${res.status} — is v${version} released with its builds?`);
  const [hash, file] = (await res.text()).trim().split(/\s+/);
  if (!/^[0-9a-f]{64}$/.test(hash) || file !== name) throw new Error(`${name}.sha256 does not name its archive`);
  return { url: `${base}/${name}`, hash };
}

// `hexscope menu` arrived after 0.7.0: earlier formulas do not point to it.
const [major, minor, patch] = version.split(".").map(Number);
const hasMenu = major > 0 || minor > 7 || (minor === 7 && patch > 0);
const caveats = hasMenu
  ? `
  def caveats
    <<~EOS
      To clean files from Finder's Quick Actions (or the Scripts menu of
      GNOME Files), run once:
        hexscope menu
    EOS
  end
`
  : "";

const block = async (os) => {
  const lines = [];
  for (const arch of ["arm", "intel"]) {
    const { url, hash } = await sha256(TARGETS[os][arch]);
    lines.push(`    on_${arch} do`, `      url "${url}"`, `      sha256 "${hash}"`, "    end");
  }
  return [`  on_${os} do`, ...lines, "  end"].join("\n");
};

process.stdout.write(`# Written by scripts/homebrew-formula.mjs in MykytaStel/hexscope from the
# checksums release v${version} carries. Edit that script, not this file.
class Hexscope < Formula
  desc "See what a file gives away before you send it, and save a clean copy"
  homepage "https://hexscope.org"
  license any_of: ["MIT", "Apache-2.0"]

${await block("macos")}

${await block("linux")}

  def install
    bin.install "hexscope"
  end
${caveats}
  test do
    assert_match "hexscope #{version}", shell_output("#{bin}/hexscope --version")
  end
end
`);
