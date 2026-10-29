Gauntlet packaged runtime (Debian package)
==========================================

Installed files
---------------
  /usr/bin/gauntlet            the standalone CLI runtime
  /usr/bin/gauntlet-workbench  the terminal operator surface (when built and
                               launch-qualified on this platform)
  /usr/share/doc/gauntlet/     this README and the licence (copyright)

No post-install script runs and no service is enabled: installing does not
start anything.

REQUIRED prerequisite: PostgreSQL
---------------------------------
`gauntlet run`, `status`, `inspect`, `verify`, `export`, and `serve` need a
PostgreSQL server. Point the GAUNTLET_DATABASE_URL environment variable at a
real server before use, for example:

  export GAUNTLET_DATABASE_URL=postgresql://gauntlet:secret@localhost:5432/gauntlet

The built-in default is a DEVELOPER instance (postgres:postgres on
127.0.0.1:54329) that does not exist on user machines — always set the
variable explicitly.

Optional capabilities (never faked)
-----------------------------------
  bubblewrap  enables non-`none` sandbox backends (`apt install bubblewrap`).
              The default backend is `none`; requesting another backend
              without bubblewrap installed is a typed refusal, never a
              silent fallback to `none`.
  bun         needed only by the codemode tool-program host. `gauntlet doctor`
              reports both as optional and never guesses.

Project and working-directory semantics
---------------------------------------
Commands resolve the Gauntlet project from the current working directory
(the directory holding GAUNTLET.md); GAUNTLET_PROJECT_ROOT overrides it.
Run the CLI from a writable project directory — `gauntlet init` scaffolds
one (GAUNTLET.md template, .env.example, .gauntlet/). Doctor reports land
under the project (redirect with GAUNTLET_REPORT_DIR). First diagnostic:

  gauntlet doctor

The workbench connects to the operator surface
----------------------------------------------
The workbench is a projection of the §20.2 operator surface — it connects
to the gateway, never bypasses it. Start the surface with the CLI:

  export GAUNTLET_API_BIND=127.0.0.1:4000   # loopback only, required
  gauntlet serve

then point the workbench at it:

  export GAUNTLET_ENDPOINT=http://127.0.0.1:4000
  gauntlet-workbench
