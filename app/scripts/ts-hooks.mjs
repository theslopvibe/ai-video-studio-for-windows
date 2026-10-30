// Lets plain Node load the app's .ts dictionaries: adds the .ts extension and stubs the Vite-only translations module.
export async function resolve(specifier, context, next) {
  if (specifier.endsWith("/translations")) return { url: "data:text/javascript,export const loc=(l,i,f)=>f;export const LANGS=[];", shortCircuit: true };
  if (specifier.startsWith("./") && !/\.\w+$/.test(specifier)) return next(specifier + ".ts", context);
  return next(specifier, context);
}
