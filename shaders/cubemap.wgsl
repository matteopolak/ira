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

