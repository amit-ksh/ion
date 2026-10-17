Pod::Spec.new do |s|
  s.name = 'IonDocumentHost'
  s.version = '1.0.0'
  s.summary = 'Scoped in-memory document resources for ion'
  s.description = s.summary
  s.license = 'MIT'
  s.author = 'ion'
  s.homepage = 'https://github.com/egoist/waku'
  s.platforms = { :ios => '16.4' }
  s.swift_version = '5.9'
  s.source = { git: s.homepage }
  s.static_framework = true
  s.dependency 'ExpoModulesCore'
  s.frameworks = 'Network'
  s.source_files = '**/*.swift'
end
