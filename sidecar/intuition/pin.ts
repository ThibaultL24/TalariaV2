// sidecar/intuition/pin.ts
// Pin Thing metadata via Intuition testnet GraphQL (pre-chain).

const DEFAULT_GRAPHQL = "https://testnet.intuition.sh/v1/graphql";

const PIN_THING = `mutation pinThing($name: String!, $description: String!, $image: String!, $url: String!) {
  pinThing(thing: { name: $name, description: $description, image: $image, url: $url }) {
    uri
  }
}`;

export async function pinThing(input: {
  name: string;
  description?: string;
  url?: string;
}): Promise<string> {
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
