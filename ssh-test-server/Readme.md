# SSH Test server

To create a Docker image that can run an SSH server and contains some generated images for testing run
```
cd ssh-test-server
docker build -t rvimage-ssh .
```
To run the container you can use
```
docker run -tid -p22:22 rvimage-ssh
```
You can connect to the container with the provided ssh-test-key-pair via
```
ssh -i test_id_rsa -p22:22 rvimage-ssh
```
and use it as test configuration in your RV Image settings.
Further, the container contains images `image1.png` to `image5.png` in
```
/home/user/test/images
```
