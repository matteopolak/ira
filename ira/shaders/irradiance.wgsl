struct VertexInput {
	@location(0) local_position: vec3<f32>,
}

@group(0) @binding(0) var t_environment: texture_2d<f32>;
@group(0) @binding(1) var s_environment: sampler;

const pi = radians(180.0);

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let normal = normalize(in.local_position);
	let irradiance = vec3<f32>(0.0);

	let right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), normal));
	let up = normalize(cross(normal, right));

	let sample_delta = 0.025;
	let samples = 0;

	for (var phi = 0.0; phi < 2.0 * pi; phi += sample_delta) {
		for (var theta = 0.0; theta < 0.5 * pi; theta += sample_delta) {
			let tangent_sample = vec3<f32>(
				sin(theta) * cos(phi),
				sin(theta) * sin(phi),
				cos(theta)
			);

			let sample = tangent_sample.x * right + tangent_sample.y * up + tangent_sample.z * normal;
			irradiance += textureSample(t_environment, s_environment, sample).rgb * cos(theta) * sin(theta);
			samples += 1;
		}
	}

	return vec4<f32>(irradiance, 1.0);
}

