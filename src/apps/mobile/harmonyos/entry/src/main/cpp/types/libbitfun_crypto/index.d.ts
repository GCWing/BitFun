declare const bitfunCrypto: {
  argon2idRaw(
    password: Uint8Array,
    salt: Uint8Array,
    memory: number,
    time: number,
    lanes: number
  ): Uint8Array;
};

export default bitfunCrypto;
