const ICNS_HEADER_LENGTH = 8;
const ICNS_CHUNK_HEADER_LENGTH = 8;

export function canonicalizeIcns(input) {
  const buffer = Buffer.from(input);

  if (
    buffer.length < ICNS_HEADER_LENGTH ||
    buffer.toString('ascii', 0, 4) !== 'icns'
  ) {
    throw new Error('Invalid ICNS header');
  }

  const declaredLength = buffer.readUInt32BE(4);
  if (declaredLength !== buffer.length) {
    throw new Error(
      `Invalid ICNS length: header declares ${declaredLength}, got ${buffer.length}`,
    );
  }

  const chunks = [];
  for (let offset = ICNS_HEADER_LENGTH; offset < buffer.length;) {
    if (offset + ICNS_CHUNK_HEADER_LENGTH > buffer.length) {
      throw new Error(`Truncated ICNS chunk header at offset ${offset}`);
    }

    const chunkLength = buffer.readUInt32BE(offset + 4);
    const chunkEnd = offset + chunkLength;
    if (chunkLength < ICNS_CHUNK_HEADER_LENGTH || chunkEnd > buffer.length) {
      throw new Error(`Invalid ICNS chunk length ${chunkLength} at offset ${offset}`);
    }

    chunks.push(buffer.subarray(offset, chunkEnd));
    offset = chunkEnd;
  }

  chunks.sort((left, right) => {
    const typeOrder = Buffer.compare(left.subarray(0, 4), right.subarray(0, 4));
    return typeOrder || Buffer.compare(left, right);
  });

  return Buffer.concat([buffer.subarray(0, ICNS_HEADER_LENGTH), ...chunks], buffer.length);
}
