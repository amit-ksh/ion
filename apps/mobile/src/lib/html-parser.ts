import {
  Parser,
  Tokenizer,
  defaultTreeAdapter as tree,
  type DefaultTreeAdapterMap,
  type Token,
} from "parse5";

export const HTML_PARSE_CHUNK = 8192;
const failure = () =>
  new Error(
    "This HTML is too complex to render. Use Reading view or export the original.",
  );

/** parse5 8.0.0 groups an arbitrarily long character run into one token.
 * Bound that optimization at input checkpoints: individual HTML5 character
 * tokens have the same tree-construction semantics. A bounded adapter also
 * avoids a giant concatenation rope across adjacent character tokens. */
class DocumentTokenizer extends Tokenizer {
  private characters: string[] = [];
  private referenceState: number | undefined;
  protected override _startCharacterReference() {
    super._startCharacterReference();
    this.referenceState = this.state;
  }
  protected override _callState(point: number) {
    super._callState(point);
    // Hibernating lookahead and the entity decoder retain input offsets. Only
    // drop consumed input after a completed state step outside a reference.
    if (
      this.active &&
      this.state !== this.referenceState &&
      this.preprocessor.willDropParsedChunk()
    )
      this.preprocessor.dropParsedChunk();
  }
  protected override _appendCharToCurrentCharacterToken(
    type: Token.CharacterToken["type"],
    character: string,
  ) {
    if (this.currentCharacterToken?.type === type) {
      this.characters.push(character);
      return;
    }
    super._appendCharToCurrentCharacterToken(type, character);
  }
  protected override _emitCurrentCharacterToken(
    location: Token.Location | null,
  ) {
    if (this.currentCharacterToken && this.characters.length) {
      this.currentCharacterToken.chars = [
        this.currentCharacterToken.chars,
        ...this.characters,
      ].join("");
      this.characters = [];
    }
    super._emitCurrentCharacterToken(location);
  }
  private attribute: { value: string } | null = null;
  private attributeParts: string[] = [];
  private attributeOwner: Token.Token | null = null;
  private comment: Token.CommentToken | null = null;
  private commentParts: string[] = [];
  private finishAttribute() {
    if (this.attribute) {
      this.attribute.value = [
        ...this.attributeParts,
        this.attribute.value,
      ].join("");
      this.attribute = null;
      this.attributeParts = [];
    }
  }
  protected override _createAttr(first: string) {
    this.finishAttribute();
    super._createAttr(first);
    this.attributeOwner = this.currentToken;
  }
  protected override _leaveAttrValue() {
    this.finishAttribute();
    super._leaveAttrValue();
  }
  protected override emitCurrentTagToken() {
    this.finishAttribute();
    super.emitCurrentTagToken();
  }
  protected override emitCurrentComment(token: Token.CommentToken) {
    if (this.comment === token) {
      token.data = [...this.commentParts, token.data].join("");
      this.comment = null;
      this.commentParts = [];
    }
    super.emitCurrentComment(token);
  }
  finishBatch() {
    this._emitCurrentCharacterToken(null);
    const token = this.currentToken;
    if (token && "attrs" in token) {
      if (
        token.tagName.length > 256 ||
        token.attrs.length > 512 ||
        this.currentAttr.name.length > 256
      )
        throw failure();
      if (this.attributeOwner === token && this.currentAttr.value.length) {
        this.attribute = this.currentAttr;
        this.attributeParts.push(this.currentAttr.value);
        this.currentAttr.value = "";
      }
    } else if (token && "data" in token) {
      this.comment = token;
      if (token.data.length) this.commentParts.push(token.data);
      token.data = "";
    } else if (
      token &&
      "publicId" in token &&
      ((token.name?.length ?? 0) > 256 ||
        (token.publicId?.length ?? 0) > HTML_PARSE_CHUNK ||
        (token.systemId?.length ?? 0) > HTML_PARSE_CHUNK)
    ) {
      throw failure();
    }
    // Input compaction belongs to completed state steps above. Batch-end
    // compaction can invalidate a pending entity decoder's saved offset.
  }
}

export function documentHtmlParser() {
  let created = 0;
  const counted = () => {
    if (++created > 50_000) throw failure();
  };
  const text = (value: string) => {
    counted();
    return tree.createTextNode(value);
  };
  const adapter = {
    ...tree,
    createElement: (...args: Parameters<typeof tree.createElement>) => {
      counted();
      if (
        args[0].length > 256 ||
        args[2].length > 512 ||
        args[2].some((attr) => attr.name.length > 256)
      )
        throw failure();
      return tree.createElement(...args);
    },
    insertText(parent: DefaultTreeAdapterMap["parentNode"], value: string) {
      const previous = parent.childNodes[parent.childNodes.length - 1];
      if (
        previous &&
        tree.isTextNode(previous) &&
        previous.value.length + value.length <= HTML_PARSE_CHUNK
      )
        previous.value += value;
      else tree.appendChild(parent, text(value));
    },
    insertTextBefore(
      parent: DefaultTreeAdapterMap["parentNode"],
      value: string,
      before: DefaultTreeAdapterMap["childNode"],
    ) {
      const previous = parent.childNodes[parent.childNodes.indexOf(before) - 1];
      if (
        previous &&
        tree.isTextNode(previous) &&
        previous.value.length + value.length <= HTML_PARSE_CHUNK
      )
        previous.value += value;
      else tree.insertBefore(parent, text(value), before);
    },
    onItemPush(node: DefaultTreeAdapterMap["parentNode"]) {
      let depth = 0,
        parent: DefaultTreeAdapterMap["parentNode"] | null = node;
      while (parent && "parentNode" in parent) {
        if (++depth > 128) throw failure();
        parent = parent.parentNode;
      }
    },
  };
  const options = { scriptingEnabled: false, treeAdapter: adapter };
  const parser = new Parser<DefaultTreeAdapterMap>(options);
  const tokenizer = new DocumentTokenizer(
    { sourceCodeLocationInfo: false },
    parser,
  );
  parser.tokenizer = tokenizer;
  return {
    document: parser.document,
    write(chunk: string, final: boolean) {
      if (chunk.length > HTML_PARSE_CHUNK)
        throw new Error("HTML parser input batch exceeds its limit");
      tokenizer.write(chunk, final);
      tokenizer.finishBatch();
    },
  };
}
