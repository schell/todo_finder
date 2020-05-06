This is another test file. The following is some garbage from my dayjob, with TODO tags sprinkled in.

To run lenz indexing from scratch you'll need nix and a checkout of takt-core. Then run

cd takt-core
nix-shell
cabal new-build apps-cli

Which will build apps-cli and place it deep in a dist dir (the dir path will be output to stdout at the end of the build)
Wait while it builds :coffee:
once we have the dir, which I'll call {result}, wec can use

// TODO: Here is an actual todo.
// It has a description.
// TODO: Here is an actual todo.
// It has a description.

{result}/apps-cli lenz build-index --help

To show some help text about how to build the index. I always run this command once before building the index because apps-cli changes.

Notice the weekstart and fiscalweek arguments. Those are important. They represent the time the user data was ingested and the week in which the experiment runs, respectively.

apps-cli expects weekstart and fiscalweek to be an ISO8601 Date, ie 2020-10-06.

Once you have the correct weekstart and fiscalweek values you can run indexing with something like

{result}/bin/apps-cli lenz build-index --host lenz-db.takt.internal --port 5432 --dbname taktdb --username takt --password password --schema conf/lenz_schema_env.json --weekstart 2020-01-06 --fiscalweek 2020-01-06

Of course your argument values will be different depending on the lenz db you're indexing on and the weekstart+fiscalweek pair.

/// TODO: Last line todo title.
/// Last line description
