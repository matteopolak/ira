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
};

struct VertexOutput {
	@builtin(position) clip_position: vec4<f32>,
	@location(0) tex_coords: vec2<f32>,
	@location(1) world_position: vec3<f32>,
	@location(2) world_normal: vec3<f32>,
	@location(3) world_tangent: vec3<f32>,
	@location(4) world_bitangent: vec3<f32>,
};

@group(1) @binding(0)
var<uniform> camera: CameraUniform;

@group(2) @binding(0)
var<uniform> light: Light;

fn calculate_tangent(normal: vec3<f32>) -> vec3<f32> {
	var helper: vec3<f32>;

	if (abs(normal.y) > 0.99) {
		helper = vec3<f32>(1.0, 0.0, 0.0);
	} else {
		helper = vec3<f32>(0.0, 1.0, 0.0);
	}

	return normalize(cross(normal, helper));
}

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
	var out: VertexOutput;

	out.clip_position = camera.view_proj * vec4<f32>(model.position, 1.0);
	out.tex_coords = model.tex_coords;
	out.world_position = model.position;
	out.world_normal = normalize(model.normal);
	// TODO: calculate tangent and bitangent when processing the model
	out.world_tangent = calculate_tangent(model.normal);
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

@group(0) @binding(8) var<uniform> has_normal: f32;

const pi = radians(180.0);
const ambient_intensity = 0.1;

fn geometry_schlick_ggx(n_dot_v: f32, k: f32) -> f32 {
	let numerator = n_dot_v;
	let denominator = n_dot_v * (1.0 - k) + k;

	return numerator / denominator;
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, k: f32) -> f32 {
	let n_dot_v = max(dot(n, v), 0.0);
	let n_dot_l = max(dot(n, l), 0.0);
	let ggx1 = geometry_schlick_ggx(n_dot_v, k);
	let ggx2 = geometry_schlick_ggx(n_dot_l, k);

	return ggx1 * ggx2;
}

fn fresnel_schlick(v_dot_h: f32, f0: vec3<f32>) -> vec3<f32> {
	return f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - v_dot_h, 5.0);
}

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, a: f32) -> f32 {
	let a2 = a * a;
	let n_dot_h = max(dot(n, h), 0.0);
	let n_dot_h2 = n_dot_h * n_dot_h;

	let numerator = a2;
	let denominator = (n_dot_h2 * (a2 - 1.0) + 1.0);

	return numerator / (pi * denominator * denominator);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let albedo = textureSample(t_diffuse, s_diffuse, in.tex_coords);
	let bump = textureSample(t_normal, s_normal, in.tex_coords).rgb;
	let metallic_roughness = textureSample(t_metallic_roughness, s_metallic_roughness, in.tex_coords);
	let ao = textureSample(t_ao, s_ao, in.tex_coords).r;

	let roughness = metallic_roughness.g;
	let metallic = metallic_roughness.b;

	// [0, 1] to [-1, 1]
	let normal = normalize((in.world_normal * (1.0 - has_normal) + bump * has_normal));
	let ambient = ambient_intensity * albedo.rgb * ao;

	// Reflectance at normal incidence (F0)
	let f0 = mix(vec3<f32>(0.04), albedo.rgb, metallic);

	// Light properties
	let light_dir = normalize(light.position - in.world_position);
	let view_dir = normalize(camera.view_pos - in.world_position);
	let half_vec = normalize(view_dir + light_dir);

	// Cook-Torrance BRDF
	let n_dot_l = max(dot(normal, light_dir), 0.0);
	let n_dot_v = max(dot(normal, view_dir), 0.0);
	let v_dot_h = dot(view_dir, half_vec);

	let d = distribution_ggx(normal, half_vec, roughness);
	let g = geometry_smith(normal, view_dir, light_dir, roughness);
	let f = fresnel_schlick(v_dot_h, f0);

	let k_s = f;
	let k_d = (vec3<f32>(1.0) - k_s) * (1.0 - metallic);

	let numerator = d * g * f;
	let denominator = 4.0 * n_dot_v * n_dot_l + 0.001; // Prevent divide by zero
	let specular = numerator / denominator;

	let radiance = light.color * light.intensity;
	let diffuse = k_d * albedo.rgb / pi;

	// Combine components
	let final_color = (diffuse + specular) * radiance * n_dot_l + ambient;

	// return vec4<f32>(normal, 1.0);
	return vec4<f32>(final_color, albedo.a);
}
