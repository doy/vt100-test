# vt100-test

This is just a basic application tying together the `vt100` and
`tokio-pty-process-stream` crates for testing purposes. Running a program with
this application should behave exactly the same as running it without (except
that currently you won't get scrollback). This is pretty much only useful for
me to be able to run all of my terminals through these two crates so that i can
notice any bugs during my day to day activities.
