def main [] {
    print "Welcome to the Cardwire's toolbox ! \n"
    print "These are scripts that i made to make my development experience more pleasent"

    print "\nScripts:"
    print "1) Release Helper"
    print "2) Database Editor"
    print "0) Exit"

    let command: int = input -s -n 1 | into int

    # Launch a script
    match $command {
        1 => {
            nu .github/scripts/bump_release.nu
        }
        2 => {
            nu .github/scripts/db_editor.nu
        }
        _ => {
            exit
        }
    }
}
