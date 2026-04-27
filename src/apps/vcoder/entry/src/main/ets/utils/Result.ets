export interface ResultInner<T, E> {
  Ok?: T;
  Err?: E;
}

export class Result<T, E> {
  Ok?: T;
  Err?: E;
  constructor(result: ResultInner<T, E>) {
    this.Ok = result.Ok;
    this.Err = result.Err;
  }
}
export function Ok<T, E>(ok?: T): Result<T, E> {
  return new Result<T, E>({ Ok: ok });
}

export function Err<T, E>(err?: E): Result<T, E> {
  return new Result<T, E>({ Err: err });
}