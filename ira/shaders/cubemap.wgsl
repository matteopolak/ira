struct VertexInput {
	@location(0) position: vec3<f32>,
}

struct VertexOutput {
	@builtin(position) clip_position: vec4<f32>,
	@location(0) local_position: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> projection: mat4x4<f32>;

@group(0) @binding(1)
var<uniform> view: mat4x4<f32>;

@group(1) @binding(0)
var<uniform> t_equirectangular: texture_2d<f32>;

@group(1) @binding(1)
var<uniform> s_equirectangular: sampler;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
	var output: VertexOutput;

	output.clip_position = projection * view * vec4<f32>(input.position, 1.0);
	output.local_position = input.position;

	return output;
}

fn importance_sample_ggx(xi: vec2<f32>, n: vec3<f32>, roughness: f32) -> f32 {
	let a = roughness * roughness;

	let phi = 2.0 * pi * xi.x;
	let cos_theta = sqrt((1.0 - xi.y) / (1.0 + (a * a - 1.0) * xi.y));
	let sin_theta = sqrt(1.0 - cos_theta * cos_theta);

	let h = vec3<f32>(
		sin_theta * cos(phi),
		sin_theta * sin(phi),
		cos_theta,
	);

	let up = abs(n.z) < 0.999 ? vec3<f32>(0.0, 0.0, 1.0) : vec3<f32>(1.0, 0.0, 0.0);
	let tangent = normalize(cross(up, n));
	let bitangent = cross(n, tangent);

	let sample = tangent * h.x + bitangent * h.y + n * h.z;

	return normalize(sample);
}

fn radical_inverse_vdc(bits: u32) -> f32 {
	var b = (bits << 16) | (bits >> 16);

	b = ((b & 0x55555555) << 1) | ((b & 0xaaaaaaaa) >> 1);
	b = ((b & 0x33333333) << 2) | ((b & 0xcccccccc) >> 2);
	b = ((b & 0x0f0f0f0f) << 4) | ((b & 0xf0f0f0f0) >> 4);
	b = ((b & 0x00ff00ff) << 8) | ((b & 0xff00ff00) >> 8);

	return f32(b) * 2.3283064365386963e-10;
}

fn hammersley(i: u32, n: u32) -> vec2<f32> {
	return vec2<f32>(f32(i) / f32(n), radical_inverse_vdc(i));
}

const inv_atan = vec2<f32>(0.1591, 0.3183);

fn sample_spherical_map(v: vec3<f32>) -> vec2<f32> {
	var uv = vec2<f32>(atan(v.z, v.x), asin(v.y));

	uv *= inv_atan;
	uv += 0.5;

	return uv;
}

@fragment
fn fs_main(in: VertexOutput) -> vec4<f32> {
	let uv = sample_spherical_map(normalize(in.local_position));
	let color = textureSample(t_equirectangular, s_equirectangular, uv).rgb;

	return vec4<f32>(color, 1.0);
}

