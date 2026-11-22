# That script will save me 2 min per month

def main [] {
    clear
    print "Welcome to the release helper for cardwire ! \n"

    let current_version = open Cargo.toml | get workspace.package.version | into semver

    mut done = false

    mut new_version = $current_version

    while done != true {
        print $"Version: ($new_version)"

        print "\nActions:"
        print "1) Bump major"
        print "2) Bump minor"
        print "3) Bump patch"
        print "4) Bump alpha"
        print "5) Bump beta"
        print "6) Bump rc"
        print "7) Apply"
        print "0) Exit"
        let command: int = input -s -n 1 | into int

        let tmp_version = match $command {
            0 => {
                exit
            }
            1 => {
                $new_version | semver bump major
            }
            2 => {
                $new_version | semver bump minor
            }
            3 => {
                $new_version | semver bump patch
            }
            4 => {
                $new_version | semver bump alpha
            }
            5 => {
                $new_version | semver bump beta
            }
            6 => {
                $new_version | semver bump rc
            }
            7 => { }
        }
        loop {
            if $command != 7 {
                print $"This will bump ($new_version) to ($tmp_version), are you sure ? [y/N]"
                let confirmation = input -n 1 -s | str lowercase

                if $confirmation like y {
                    $new_version = $tmp_version
                    break
                } else if $confirmation like n {
                    break
                } else {
                    print "unknown input"
                }
            }
            # Apply to all files
            if $command == 7 {
                let text_version = $new_version | to text
                print "Applying to Cargo.toml..."
                open Cargo.toml | upsert workspace.package.version $text_version | save -f Cargo.toml
                print "Cargo.toml OK"

                print "Applying to appstream..."

                open --raw assets/org.opengamingcollective.cardwire.metainfo.xml
                | str replace --regex '<release version="[^"]*"' ('<release version="' + $text_version + '"')
                | save -f assets/org.opengamingcollective.cardwire.metainfo.xml
                print "Appstream OK"

                print "Applying to arch packages..."

                # Arch needs a "_" instead of a "-"
                let arch_version = $text_version | str replace "-" "_"

                open --raw packages/arch-linux/cardwire-PKGBUILD
                | str replace --regex --multiline '^_tag=.*' ('_tag=' + $text_version)
                | str replace --regex --multiline '^pkgver=.*' ('pkgver=' + $arch_version)
                | save -f packages/arch-linux/cardwire-PKGBUILD

                print "cardwire-PKGBUILD OK"

                open --raw packages/arch-linux/cardwire-nix-PKGBUILD
                | str replace --regex --multiline '^_tag=.*' ('_tag=' + $text_version)
                | str replace --regex --multiline '^pkgver=.*' ('pkgver=' + $arch_version)
                | save -f packages/arch-linux/cardwire-nix-PKGBUILD

                print "cardwire-nix-PKGBUILD OK"

                print "All versions have been bumped"

                print "git commit + git tag ? [y/N]"
                loop {
                    let confirmation = input -n 1 -s | str lowercase

                    if $confirmation like y {
                        print "Running git add..."
                        git add Cargo.toml Cargo.lock assets/org.opengamingcollective.cardwire.metainfo.xml packages/arch-linux/cardwire-nix-PKGBUILD packages/arch-linux/cardwire-PKGBUILD
                        print "Running git commit..."
                        git commit -m $"chore: bump to v($text_version)"
                        print "Running git tag..."
                        git tag -a $"v($text_version)" -m $"v($text_version)"
                        print "Confirmation"
                        print "Git log:"
                        git log -n 3
                        print "Git tag"
                        print (git tag | lines | sort -n | reverse)

                        break
                    } else if $confirmation like n {
                        break
                    } else {
                        print "unknown input"
                    }
                }

                exit
            }
        }

        clear
    }
}
