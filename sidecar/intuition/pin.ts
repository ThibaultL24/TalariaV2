// sidecar/intuition/pin.ts
// Pin Thing metadata → ipfs:// URI (Pinata when JWT set, else Intuition GraphQL).

const DEFAULT_GRAPHQL = "https://testnet.intuition.sh/v1/graphql";
const PINATA_PIN_JSON = "https://api.pinata.cloud/pinning/pinJSONToIPFS";

const PIN_THING = `mutation pinThing($name: String!, $description: String!, $image: String!, $url: String!) {
  pinThing(thing: { name: $name, description: $description, image: $image, url: $url }) {
    uri
  }
}`;

export type PinThingInput = {
  name: string;
  description?: string;
  url?: string;
};

/** Prefer Pinata when `PINATA_JWT` is set; otherwise Intuition testnet GraphQL pinThing. */
export async function pinThing(input: PinThingInput): Promise<string> {
  const jwt = process.env["PINATA_JWT"]?.trim();
  if (jwt) {
    return pinViaPinata(jwt, input);
  }
  return pinViaIntuitionGraphql(input);
}

async function pinViaPinata(jwt: string, input: PinThingInput): Promise<string> {
  const content = {
    name: input.name,
    description: input.description ?? "",
    image: "",
    url: input.url ?? "",
    "@type": "Thing",
  };
  const res = await fetch(PINATA_PIN_JSON, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      authorization: `Bearer ${jwt}`,
    },
    body: JSON.stringify({
      pinataContent: content,
      pinataMetadata: { name: input.name.slice(0, 100) || "talaria-thing" },
    }),
  });
  if (!res.ok) {
    const detail = await res.text().catch(() => "");
    throw new Error(`pinata HTTP ${res.status}${detail ? `: ${detail.slice(0, 200)}` : ""}`);
  }
  const body = (await res.json()) as { IpfsHash?: string };
  const hash = body.IpfsHash?.trim() ?? "";
  if (!hash) {
    throw new Error("pinata did not return IpfsHash");
  }
  return `ipfs://${hash}`;
}

async function pinViaIntuitionGraphql(input: PinThingInput): Promise<string> {
  const endpoint =
    process.env["INTUITION_GRAPHQL"]?.trim() || DEFAULT_GRAPHQL;
  const res = await fetch(endpoint, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      query: PIN_THING,
      variables: {
        name: input.name,
        description: input.description ?? "",
        image: "",
        url: input.url ?? "",
      },
    }),
  });
  if (!res.ok) {
    throw new Error(`pin HTTP ${res.status}`);
  }
  const body = (await res.json()) as {
    errors?: { message: string }[];
    data?: { pinThing?: { uri?: string } };
  };
  if (body.errors?.length) {
    throw new Error(body.errors.map((e) => e.message).join("; "));
  }
  const uri = body.data?.pinThing?.uri ?? "";
  if (!uri.startsWith("ipfs://")) {
    throw new Error("pin did not return ipfs:// uri");
  }
  return uri;
}

export function atomDataFromPinUri(uri: string): string {
  const trimmed = uri.trim();
  if (!trimmed.startsWith("ipfs://")) {
    throw new Error("expected ipfs:// uri");
  }
  return trimmed;
}
