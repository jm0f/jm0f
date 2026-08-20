# Where the sounds came from

Nine sounds, all by [Kenney](https://kenney.nl), all **CC0**. That licence is
why they can live in this repository at all: the files are redistributed inside
the binary, and the page makes no external requests to fetch them.

| File | Kenney pack | Original |
|---|---|---|
| `confirmation-001.mp3` | Interface Sounds | `confirmation_001.ogg` |
| `confirmation-002.mp3` | Interface Sounds | `confirmation_002.ogg` |
| `dice-throw-3.mp3` | Casino Audio | `dice-throw-3.ogg` |
| `card-place-1.mp3` | Casino Audio | `card-place-1.ogg` |
| `impact-generic-light-002.mp3` | Impact Sounds | `impactGeneric_light_002.ogg` |
| `drop-002.mp3` | Interface Sounds | `drop_002.ogg` |
| `error-008.mp3` | Interface Sounds | `error_008.ogg` |
| `jingles-hit-10.mp3` | Music Jingles | `jingles_HIT10.ogg` |
| `jingles-hit-15.mp3` | Music Jingles | `jingles_HIT15.ogg` |

Kenney numbers these from zero, so "Light 2" is the file called `002`, the one
soundcn titles *Impact Generic Light 002*, and not the second of the five. The
jingles are numbered `00` to `16` in two digits.

Taken by way of [soundcn](https://github.com/kapishdima/soundcn), which is a
registry of these packs cut into named, individually installable sounds. Its own
delivery is a shadcn CLI command that writes a TypeScript module per sound, for
a React project with a `useSound` hook. None of that applies here: this is a
dependency-free Rust binary serving one static page. So what was taken is the
audio, which is CC0 and Kenney's, and the naming, which is soundcn's and is what
each of them was called in the conversation that asked for it.

Its install command is `pnpm dlx shadcn add @soundcn/<name>`, which does not
apply here and was tried once to be sure: with no `components.json` it stops to
ask which React component library to scaffold, which is the wrong question to
answer in a Rust workspace. What the command would fetch is served plainly at
`https://soundcn.xyz/r/<name>.json`, so that is what is read, and the MP3 is
decoded out of the `dataUri` field of the TypeScript module inside it.

The MP3s are soundcn's transcodes of Kenney's OGG originals, lifted out of the
registry's base64 data URIs. MP3 rather than OGG because every browser plays it,
including the Safari versions that do not play Vorbis, and the difference is a
few kilobytes on a file that is already under six.

`CC0-Kenney.txt` is Kenney's own licence note, shipped beside the files the way
the fonts' OFL copies are. CC0 does not require attribution; Kenney asks for it
and it costs nothing, so it is here and in `docs/style.md`.
