import { gzipSync } from "node:zlib";
import { chmod, readFile, writeFile } from "node:fs/promises";
import { basename, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const BLOCK_SIZE = 512;

function writeOctal(header, offset, length, value) {
  const octal = value.toString(8).padStart(length - 1, "0");
  if (octal.length >= length) {
    throw new Error(`tar value ${String(value)} does not fit in ${String(length)} bytes`);
  }
  header.write(octal, offset, length - 1, "ascii");
  header[offset + length - 1] = 0;
}

function tarEntry(name, contents, mode) {
  const nameBytes = Buffer.from(name, "utf8");
  if (
    nameBytes.length > 100 ||
    name.includes("/") ||
    name.includes("\\") ||
    name === "" ||
    name === "." ||
    name === ".." ||
    name.normalize("NFC") !== name ||
    [...name].some((character) => /[\u0000-\u001f\u007f]/u.test(character))
  ) {
    throw new Error(`unsupported release archive entry name ${JSON.stringify(name)}`);
  }
  const header = Buffer.alloc(BLOCK_SIZE);
  nameBytes.copy(header, 0);
  writeOctal(header, 100, 8, mode);
  writeOctal(header, 108, 8, 0);
  writeOctal(header, 116, 8, 0);
  writeOctal(header, 124, 12, contents.length);
  writeOctal(header, 136, 12, 0);
  header.fill(0x20, 148, 156);
  header[156] = "0".charCodeAt(0);
  header.write("ustar\0", 257, 6, "ascii");
  header.write("00", 263, 2, "ascii");
  header.write("root", 265, 4, "ascii");
  header.write("root", 297, 4, "ascii");
  writeOctal(header, 329, 8, 0);
  writeOctal(header, 337, 8, 0);
  const checksum = header.reduce((sum, byte) => sum + byte, 0);
  const checksumText = checksum.toString(8).padStart(6, "0");
  header.write(checksumText, 148, 6, "ascii");
  header[154] = 0;
  header[155] = 0x20;
  const padding = Buffer.alloc(
    (BLOCK_SIZE - (contents.length % BLOCK_SIZE)) % BLOCK_SIZE,
  );
  return Buffer.concat([header, contents, padding]);
}

export async function createReleaseArchive(output, inputs) {
  const names = new Set();
  const entries = [];
  for (const input of inputs) {
    const path = resolve(input.path);
    const name = input.name ?? basename(path);
    if (names.has(name)) {
      throw new Error(`duplicate release archive entry ${name}`);
    }
    names.add(name);
    entries.push({
      contents: await readFile(path),
      mode: input.executable ? 0o755 : 0o644,
      name,
    });
  }
  entries.sort((left, right) => Buffer.compare(Buffer.from(left.name), Buffer.from(right.name)));
  const tar = entries.map(({ contents, mode, name }) => tarEntry(name, contents, mode));
  tar.push(Buffer.alloc(BLOCK_SIZE * 2));
  const archive = gzipSync(Buffer.concat(tar), { level: 9, mtime: 0 });
  await writeFile(output, archive, { mode: 0o644 });
  await chmod(output, 0o644);
}

async function main() {
  const [output, binary, ...legalFiles] = process.argv.slice(2);
  if (!output || !binary || legalFiles.length === 0) {
    throw new Error(
      "usage: create-release-archive.mjs <output.tar.gz> <binary> <legal-file>...",
    );
  }
  await createReleaseArchive(output, [
    { executable: true, path: binary },
    ...legalFiles.map((path) => ({ path })),
  ]);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
