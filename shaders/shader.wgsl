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

struct InstanceInput {
	@location(3) model_matrix_0: vec4<f32>,
	@location(4) model_matrix_1: vec4<f32>,
	@location(5) model_matrix_2: vec4<f32>,
	@location(6) model_matrix_3: vec4<f32>,
}

struct VertexOutput {
	@builtin(position) clip_position: vec4<f32>,
	@location(0) tex_coords: vec2<f32>,
	@location(1) world_normal: vec3<f32>,
	@location(2) world_tangent: vec3<f32>,
	@location(3) world_bitangent: vec3<f32>,
	@location(4) world_position: vec3<f32>,
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
fn vs_main(model: VertexInput, instance: InstanceInput) -> VertexOutput {
	let model_matrix = mat4x4<f32>(
		instance.model_matrix_0,
		instance.model_matrix_1,
		instance.model_matrix_2,
		instance.model_matrix_3,
	);

	var out: VertexOutput;

	out.tex_coords = model.tex_coords;
	out.clip_position = camera.view_proj * model_matrix * vec4<f32>(model.position, 1.0);
	out.world_normal = normalize((model_matrix * vec4<f32>(model.normal, 0.0)).xyz);
	out.world_tangent = calculate_tangent(out.world_normal);
	out.world_bitangent = cross(out.world_normal, out.world_tangent);
	out.world_position = (model_matrix * vec4<f32>(model.position, 1.0)).xyz;

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

fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
	let r = roughness + 1.0;
	let k = (r * r) / 8.0;

	let numerator = n_dot_v;
	let denominator = n_dot_v * (1.0 - k) + k;

	return numerator / denominator;
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
	let n_dot_v = max(dot(n, v), 0.0);
	let n_dot_l = max(dot(n, l), 0.0);
	let ggx1 = geometry_schlick_ggx(n_dot_v, roughness);
	let ggx2 = geometry_schlick_ggx(n_dot_l, roughness);

	return ggx1 * ggx2;
}

fn fresnel_schlick(h_dot_v: f32, f0: vec3<f32>) -> vec3<f32> {
	return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - h_dot_v, 0.0, 1.0), 5.0);
}

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
	let a = roughness * roughness;
	let a2 = a * a;
	let n_dot_h = max(dot(n, h), 0.0);
	let n_dot_h2 = n_dot_h * n_dot_h;

	let numerator = a2;
	let denominator = (n_dot_h2 * (a2 - 1.0) + 1.0);

	return numerator / (pi * denominator * denominator);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let albedo_raw = textureSample(t_diffuse, s_diffuse, in.tex_coords);
	let albedo = pow(albedo_raw.rgb, vec3<f32>(2.2));
	let bump = textureSample(t_normal, s_normal, in.tex_coords).rgb;
	let metallic_roughness = textureSample(t_metallic_roughness, s_metallic_roughness, in.tex_coords);
	let ao = textureSample(t_ao, s_ao, in.tex_coords).r;

	let roughness = metallic_roughness.g;
	let metallic = metallic_roughness.b;

	// Reflectance at normal incidence (F0)
	let f0 = mix(vec3<f32>(0.04), albedo.rgb, metallic);

	let n = normalize((in.world_normal * (1.0 - has_normal) + bump * (has_normal)));
	let v = normalize(camera.view_pos - in.world_position);

	var lo = vec3<f32>(0.0);

	for (var i = 0; i < 1; i++) {
		let l = normalize(light.position - in.world_position);
		let h = normalize(v + l);
		let distance = length(light.position - in.world_position);
		let attenuation = 1.0 / (distance * distance);
		let radiance = light.color * light.intensity * attenuation;

		// cook-torrance brdf
		let f = fresnel_schlick(max(dot(h, v), 0.0), f0);
		let ndf = distribution_ggx(n, h, roughness);
		let g = geometry_smith(n, v, l, roughness);

		let numerator = ndf * g * f;
		let denominator = 4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 0.0001;
		let specular = numerator / denominator;

		// calculate ratio of refraction
		let k_s = f;
		let k_d = (vec3<f32>(1.0) - k_s) * (1.0 - metallic);

		let n_dot_l = max(dot(n, l), 0.0);

		// add to outgoing radiance Lo
		lo += (k_d * albedo / pi + specular) * radiance * n_dot_l;
	}

	// [0, 1] to [-1, 1]
	let ambient = vec3<f32>(0.03) * albedo * ao;

	var color = ambient + lo;

	// tone mapping
	color = color / (color + vec3<f32>(1.0));
	color = pow(color, vec3<f32>(1.0 / 2.2));

	return vec4<f32>(color, albedo_raw.a);
}

