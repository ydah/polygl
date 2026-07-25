export const swiftShaderLaunchOptions = {
  headless: true,
  args: [
    "--enable-unsafe-swiftshader",
    "--enable-webgl",
    "--ignore-gpu-blocklist",
    "--use-angle=swiftshader-webgl",
    "--use-gl=angle",
  ],
};

export const viewport = { width: 320, height: 240 };
