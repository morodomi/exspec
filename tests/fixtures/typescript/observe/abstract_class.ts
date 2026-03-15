export abstract class BaseService {
  abstract findAll(): any[];

  validate(data: any): boolean {
    return data != null;
  }
}

abstract class InternalBase {
  process(data: any): any {
    return data;
  }
}
