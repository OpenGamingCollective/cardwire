def main [] {
    let user = whoami

    let should_elevate = $user != root

    if $should_elevate {
        print $"user is not root, elevating privileges..."
        sleep 1sec
        let nu_bin = which nu | get 0.path
        ^sudo $nu_bin $env.CURRENT_FILE
        exit $env.LAST_EXIT_CODE
    }
    clear
    print "Welcome to the Database Editor for Cardwire ! \n"

    let path = "/var/lib/cardwire/cardwire.db"

    if ($path | path exists) == false {
        print $"($path) doesn't not exist, exiting..."
    }

    open $path
}
