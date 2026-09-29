// TypeScript service with types that stress the ASI-aware joiner.
export interface User {
  id: number;
  name: string;
  email?: string;
  readonly roles: ReadonlyArray<Role>;
}

export type Role =
  | "admin"
  | "editor"
  | "viewer";

enum Level {
  Low = 1,
  High = Low << 3,
}

type Mapper<T, U> = (value: T, index: number) => U;

abstract class Repo<T extends { id: number }> {
  protected items = new Map<number, T>();
  abstract validate(item: T): boolean;

  save(item: T): this {
    if (!this.validate(item)) {
      throw new Error(`invalid ${item.id}`);
    }
    this.items.set(item.id, item);
    return this;
  }

  map<U>(fn: Mapper<T, U>): U[] {
    return [...this.items.values()].map(fn);
  }
}

class UserRepo extends Repo<User> {
  validate(u: User): boolean {
    return u.name.length > 0 && u.roles.length > 0;
  }
}

namespace Util {
  export const clamp = (n: number, lo = 0, hi = 10): number =>
    Math.min(hi, Math.max(lo, n));
}

function assertNever(x: never): never {
  throw new Error("unexpected " + x);
}

const repo = new UserRepo()
  .save({ id: 1, name: "ann", roles: ["admin"] })
  .save({ id: 2, name: "bob", roles: ["viewer", "editor"] });

const names = repo.map((u, i) => `${i}:${u.name}`);
const cfg = { retries: 3, verbose: false } as const;
const total = names.length * Level.High - -1;
const maybe: string | undefined = Math.random() > 2 ? "x" : undefined;
const len = maybe?.length ?? -1;
const gen = <T,>(x: T): T[] => [x, x];
console.log(names.join(","), cfg.retries, total, len, Util.clamp(42), gen<number>(7), Level.High);
