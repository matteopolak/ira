struct CameraUniform {
	view_proj: mat4x4<f32>,
	view_pos: vec3<f32>,
};

struct Light {
	position: vec3<f32>,
	color: vec3<f32>,
	intensity: f32,
};

struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) normal: vec3<f32>,
	@location(2) tex_coords: vec2<f32>,
	@location(3) diffuse_tex_coords: vec2<f32>,
};

struct VertexOutput {
	@builtin(position) clip_position: vec4<f32>,
	@location(0) tex_coords: vec2<f32>,
	@location(1) diffuse_tex_coords: vec2<f32>,
	@location(2) world_position: vec3<f32>,
	@location(3) world_normal: vec3<f32>,
	@location(4) world_tangent: vec3<f32>,
	@location(5) world_bitangent: vec3<f32>,
};

@group(1) @binding(0)
var<uniform> camera: CameraUniform;

@group(2) @binding(0)
var<uniform> light: Light;

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
	var out: VertexOutput;

	out.clip_position = camera.view_proj * vec4<f32>(model.position, 1.0);
	out.tex_coords = model.tex_coords;
	out.diffuse_tex_coords = model.diffuse_tex_coords;
	out.world_position = model.position;
	out.world_normal = model.normal;
	// TODO: calculate tangent and bitangent when processing the model
	out.world_tangent = vec3<f32>(1.0, 0.0, 0.0);
	out.world_bitangent = cross(out.world_normal, out.world_tangent);

	return out;
}

@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;

@group(0) @binding(2) var t_normal: texture_2d<f32>;
@group(0) @binding(3) var s_normal: sampler;

@group(0) @binding(4) var t_metallic_roughness: texture_2d<f32>;
@group(0) @binding(5) var s_metallic_roughness: sampler;

@group(0) @binding(6) var t_ao: texture_2d<f32>;
@group(0) @binding(7) var s_ao: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let base_color = textureSample(t_diffuse, s_diffuse, in.diffuse_tex_coords);
	let normal_map = textureSample(t_normal, s_normal, in.tex_coords).rgb;
	let metallic_roughness = textureSample(t_metallic_roughness, s_metallic_roughness, in.tex_coords);
	let ao = textureSample(t_ao, s_ao, in.tex_coords).r;

	let roughness = metallic_roughness.r;
	let metallic = metallic_roughness.b;

	let N_tangent = normalize(normal_map * 2.0 - 1.0);
	let TBN = mat3x3<f32>(in.world_tangent, in.world_bitangent, in.world_normal);
	let N = normalize(TBN * N_tangent);

	let V = normalize(camera.view_pos - in.world_position);
	let ambient_intensity = 0.3;

	var final_color = base_color.rgb * (ambient_intensity * ao);

	let L = normalize(light.position - in.world_position);
	let H = normalize(L + V);

	let diff = max(dot(N, L), 0.0);
	let diffuse = diff * light.color * base_color.rgb;

	let spec = pow(max(dot(N, H), 0.0), 32.0);
	let specular = spec * light.color * metallic;

	final_color += (diffuse + specular) * light.intensity;
	final_color = clamp(final_color, vec3<f32>(0.0), vec3<f32>(1.0));

	return vec4<f32>(final_color, base_color.a);
}
