# Where the sounds came from

Four sounds, all by [Kenney](https://kenney.nl), all **CC0**. That licence is
why they can live in this repository at all: the files are redistributed inside
the binary, and the page makes no external requests to fetch them.

| File | Kenney pack | Original |
|---|---|---|
| `dice-throw-3.mp3` | Casino Audio | `dice-throw-3.ogg` |
| `card-place-1.mp3` | Casino Audio | `card-place-1.ogg` |
| `impact-generic-light-002.mp3` | Impact Sounds | `impactGeneric_light_002.ogg` |
| `drop-002.mp3` | Interface Sounds | `drop_002.ogg` |

Kenney numbers these from `000`, so "Light 2" is the file called `002`, the one
soundcn titles *Impact Generic Light 002*, and not the second of the five.

Taken by way of [soundcn](https://github.com/kapishdima/soundcn), which is a
registry of these packs cut into named, individually installable sounds. Its own
delivery is a shadcn CLI command that writes a TypeScript module per sound, for
a React project with a `useSound` hook. None of that applies here: this is a
dependency-free Rust binary serving one static page. So what was taken is the
audio, which is CC0 and Kenney's, and the naming, which is soundcn's and is what
these three are called in the conversation that asked for them.

The MP3s are soundcn's transcodes of Kenney's OGG originals, lifted out of the
registry's base64 data URIs. MP3 rather than OGG because every browser plays it,
including the Safari versions that do not play Vorbis, and the difference is a
few kilobytes on a file that is already under six.

`CC0-Kenney.txt` is Kenney's own licence note, shipped beside the files the way
the fonts' OFL copies are. CC0 does not require attribution; Kenney asks for it
and it costs nothing, so it is here and in `docs/style.md`.
